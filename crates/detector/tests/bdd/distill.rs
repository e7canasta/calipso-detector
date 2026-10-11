//! Drive the core and publish its data before trying to distill them.
use crate::world::Shell;
use cucumber::{given, then, when};
use detector::publish::{Kind, PublisherHandle};
use golden::{distill::distill, language::Sense, world};
use scene_core::{
    Input, Monitor,
    contract::{Build, Profile, ProfileHash, SampleRef, SessionId, Source, SourceSecs, Timing},
    geom::Quad,
    look::{Failure, Probe, Reading, Spent},
    map::SiteMap,
    policy::{FacePolicy, Policy, ReinforcePolicy},
};

fn site() -> SiteMap {
    let mut map = SiteMap::new(Quad::from_rect(world::BED));
    map.resolution = Some(world::FRAME);
    map.headboard = Some(world::HEADBOARD);
    map
}

fn sample(n: u64) -> Input {
    Input::Sample {
        at: SampleRef {
            session: SessionId(1),
            sequence: n,
            source_secs: SourceSecs(n as f64),
        },
        frame: world::FRAME,
    }
}

fn night(
    w: &mut Shell,
    policy: Policy,
    mut respond: impl FnMut(u64, &Probe) -> Result<Reading, Failure>,
) {
    let mut monitor = Monitor::new(policy.clone(), site());
    w.policy = Some(policy);
    w.night = (1..=8)
        .map(|n| {
            monitor
                .handle(sample(n))
                .drive(|r| (respond(n, r), Spent(1.0)))
        })
        .collect();
}

fn reinforcement() -> Policy {
    Policy {
        reinforce: Some(ReinforcePolicy { bed_padding: 0.1 }),
        ..Policy::default()
    }
}

#[given(expr = "a night with two bodies in bed where {string} wins the shared box in sample 4")]
fn shared_boxes(w: &mut Shell, winner: String) {
    assert!(matches!(winner.as_str(), "main" | "reinforcement"));
    night(w, reinforcement(), |n, r| {
        let mut body = world::in_bed(0);
        let boxes = match r {
            Probe::Detect { .. } => {
                body.confidence = if winner == "main" { 0.9 } else { 0.7 };
                if n <= 3 {
                    vec![body, world::in_bed(1)]
                } else {
                    vec![body]
                }
            }
            Probe::Reinforce { .. } => {
                body.confidence = if winner == "reinforcement" { 0.9 } else { 0.7 };
                vec![body]
            }
            _ => panic!("unexpected probe"),
        };
        Ok(Reading::Bodies(boxes))
    });
}

#[given(expr = "a night where {string} fails in sample 4")]
fn sense_fails(w: &mut Shell, sense: String) {
    let faces = sense == "faces";
    assert!(faces || sense == "reinforcement");
    let policy = if faces {
        Policy {
            faces: Some(FacePolicy {
                min_confidence: 0.3,
            }),
            ..Policy::default()
        }
    } else {
        reinforcement()
    };
    night(w, policy, |n, r| match r {
        Probe::Detect { .. } => Ok(Reading::Bodies(if faces || n <= 3 {
            vec![world::in_bed(0)]
        } else {
            vec![]
        })),
        Probe::Reinforce { .. } | Probe::Faces { .. } if n == 4 => Err(Failure::Model),
        Probe::Reinforce { .. } => Ok(Reading::Bodies(vec![world::in_bed(0)])),
        Probe::Faces { .. } => Ok(Reading::Faces(vec![])),
        _ => panic!("unexpected probe"),
    });
}

#[given("a night where reinforcement looked without finding bodies")]
fn empty_reinforcement(w: &mut Shell) {
    night(w, reinforcement(), |n, r| match r {
        Probe::Detect { .. } => Ok(Reading::Bodies(if n <= 3 {
            vec![world::in_bed(0)]
        } else {
            vec![]
        })),
        Probe::Reinforce { .. } => Ok(Reading::Bodies(vec![])),
        _ => panic!("unexpected probe"),
    });
}

#[given("a night where faces looked without finding any")]
fn empty_faces(w: &mut Shell) {
    let policy = Policy {
        faces: Some(FacePolicy {
            min_confidence: 0.3,
        }),
        ..Policy::default()
    };
    night(w, policy, |_, r| match r {
        Probe::Detect { .. } => Ok(Reading::Bodies(vec![world::in_bed(0)])),
        Probe::Faces { .. } => Ok(Reading::Faces(vec![])),
        _ => panic!("unexpected probe"),
    });
}

#[when("the night is distilled")]
fn distill_night(w: &mut Shell) {
    let (mut publisher, _rx) = PublisherHandle::channel(
        Source {
            site: "test".into(),
            camera: "bed".into(),
            run_id: "regression".into(),
            build: Build {
                version: "test".into(),
                git: "test".into(),
            },
        },
        Profile {
            camera: None,
            resident: None,
            hash: ProfileHash("test".into()),
            senses: vec![],
        },
    );
    let mut jsonl = String::new();
    for out in &w.night {
        for m in publisher
            .prepare(
                out.clone(),
                Timing {
                    wait_ms: 0.0,
                    processing_ms: 0.0,
                },
            )
            .messages()
            .unwrap()
        {
            if m.kind == Kind::Scene {
                jsonl.push_str(std::str::from_utf8(&m.bytes).unwrap());
                jsonl.push('\n');
            }
        }
    }
    w.distilled = Some(distill(&jsonl, None, Some(2.0)));
}

#[then(expr = "the distillation rejects sample {int} with a reason that names {string}")]
fn rejected(w: &mut Shell, n: u64, name: String) {
    let error = w.distilled.as_ref().unwrap().as_ref().unwrap_err();
    assert_eq!(error.line, Some(n as usize));
    let message = error.to_string();
    assert!(message.contains(&format!("sample {n}")), "{message}");
    assert!(message.contains(&name), "{message}");
}

#[then("the distillation keeps the reinforcement sense")]
fn keeps_reinforcement(w: &mut Shell) {
    let d = w.distilled.as_ref().unwrap().as_ref().unwrap();
    assert_eq!(d.scene.senses, vec![Sense::Detect, Sense::Reinforce]);
}

#[then("the distillation keeps the face sense")]
fn keeps_faces(w: &mut Shell) {
    let d = w.distilled.as_ref().unwrap().as_ref().unwrap();
    assert_eq!(d.scene.senses, vec![Sense::Detect, Sense::Faces]);
}

#[then("the distilled night repeats the same scenes and events per sample")]
fn repeats(w: &mut Shell) {
    let d = w.distilled.as_ref().unwrap().as_ref().unwrap();
    // The composition picks the policies according to the declared senses.
    let mut policy = w.policy.clone().unwrap();
    if !d.scene.senses.contains(&Sense::Reinforce) {
        policy.reinforce = None;
    }
    if !d.scene.senses.contains(&Sense::Faces) {
        policy.faces = None;
    }
    let mut monitor = Monitor::new(policy, site());
    let replayed: Vec<_> = d
        .scene
        .samples()
        .iter()
        .map(|m| {
            monitor
                .handle(sample(u64::from(m.n)))
                .drive(|r| (world::respond(m, r), Spent(1.0)))
        })
        .collect();
    assert_eq!(replayed, w.night);
}
