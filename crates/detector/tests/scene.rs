//! R7-3: a golden scene runs through the real detector (scene source, scripted
//! senses, driver, publisher) and gives exactly the same as the loop driven
//! directly with the same scene, as the BDD harness does. If they ever differ,
//! there is logic in the shell that should not be there.
#![allow(clippy::disallowed_methods)]

use detector::driver::Driver;
use detector::senses::Scripted;
use detector::source::scene::{Pace, SceneSource};
use golden::language::Scene;
use golden::world;
use scene_core::contract::{SampleRef, SensorState, SessionId, SourceSecs};
use scene_core::geom::Quad;
use scene_core::look::Spent;
use scene_core::map::SiteMap;
use scene_core::policy::Policy;
use scene_core::{Input, Monitor, Output};
use std::time::Duration;
use tokio_util::sync::CancellationToken;

const S1: &str = include_str!("../../../golden/s1-staff-put-to-bed.md");

fn site() -> SiteMap {
    let mut map = SiteMap::new(Quad::from_rect(world::BED));
    map.resolution = Some(world::FRAME);
    map.headboard = Some(world::HEADBOARD);
    map
}

/// Through the detector: the scene source and the scripted senses.
fn through_the_detector(scene: &Scene) -> Vec<Output> {
    let mut driver = Driver::new(Policy::default(), site(), Scripted, Duration::from_secs(10));
    let mut source = SceneSource::new(scene, Pace::Fast);
    let mut outputs = Vec::new();
    driver.run(&mut source, &CancellationToken::new(), |p| {
        outputs.push(p.output)
    });
    outputs
}

/// Direct: the loop driven like the BDD harness, with the same fake.
fn direct(scene: &Scene) -> Vec<Output> {
    direct_with(scene, Policy::default(), site())
}

fn direct_with(scene: &Scene, policy: Policy, site: SiteMap) -> Vec<Output> {
    let mut monitor = Monitor::new(policy, site);
    let connects = SampleRef {
        session: SessionId(1),
        sequence: 0,
        source_secs: SourceSecs(0.0),
    };
    let mut outputs = vec![
        monitor
            .handle(Input::Sensor {
                at: connects,
                state: SensorState::Streaming,
            })
            .cancel(),
    ];
    for m in scene.samples() {
        let at = SampleRef {
            session: SessionId(1),
            sequence: u64::from(m.n),
            source_secs: SourceSecs(m.source_secs),
        };
        outputs.push(
            monitor
                .handle(Input::Sample {
                    at,
                    frame: world::FRAME,
                })
                .drive(|r| (world::respond(&m, &r.probe), Spent(0.0))),
        );
    }
    outputs
}

#[test]
fn s1_through_the_detector_equals_the_direct_loop() {
    let scene = Scene::parse(S1).unwrap();
    let detector = through_the_detector(&scene);
    let direct = direct(&scene);
    assert_eq!(detector.len(), direct.len());
    for (a, b) in detector.iter().zip(&direct) {
        assert_eq!(a, b, "they differ at {:?}", a.at);
    }
}

mod contract_of_the_fake {
    //! M-3 of the R7 design table: the fake is tested against the real one. A
    //! "perfect model" sees exactly the boxes whose centre falls in the crop it
    //! receives, in its local coordinates; `sense::Senses` crops and takes them
    //! to the frame. For each probe, the real one with perfect models and the
    //! fake answer the same.
    use super::*;
    use detector::senses::Senses;
    use golden::language::{Expected, Faces, InBed, Location, Samples, Span};
    use image::DynamicImage;
    use scene_core::geom::Rect;
    use scene_core::look::{Detection, FaceRegion, LookId, Probe, Request};
    use std::{cell::Cell, rc::Rc};

    struct Perfect {
        world: Vec<Detection>,
        /// The origin of the current crop: the model does not know it, the test does.
        origin: Rc<Cell<(f32, f32)>>,
    }

    impl sense::DetectorPort for Perfect {
        fn detect(&mut self, image: &DynamicImage) -> Result<Vec<Detection>, sense::ModelError> {
            let (ox, oy) = self.origin.get();
            let (w, h) = (image.width() as f32, image.height() as f32);
            Ok(self
                .world
                .iter()
                .filter(|d| {
                    let c = d.bbox.centroid();
                    c.x >= ox && c.x <= ox + w && c.y >= oy && c.y <= oy + h
                })
                .map(|d| Detection {
                    bbox: Rect::new(
                        d.bbox.x0 - ox,
                        d.bbox.y0 - oy,
                        d.bbox.x1 - ox,
                        d.bbox.y1 - oy,
                    ),
                    ..*d
                })
                .collect())
        }
    }

    fn sample(faces: Faces) -> world::Sample {
        let span = Span {
            samples: Samples { from: 1, to: 1 },
            floor: 1,
            location: Location::Door,
            bed: InBed::Fixed(1),
            reinforcement: 1,
            faces,
            expected: Expected::default(),
        };
        world::samples(&[span], 1.0).remove(0)
    }

    #[test]
    fn the_fake_answers_what_the_real_sense_answers_with_perfect_models() {
        for faces in [Faces::OnSubject, Faces::AtHeadboard] {
            let m = sample(faces);
            let origin = Rc::new(Cell::new((0.0, 0.0)));
            let perfect = |world: &[Detection]| Perfect {
                world: world.to_vec(),
                origin: Rc::clone(&origin),
            };
            let mut real = sense::Senses::new(perfect(&m.detect))
                .with_reinforce(perfect(&m.reinforce))
                .with_faces(perfect(&m.faces));
            let image = DynamicImage::new_rgb8(world::FRAME.width, world::FRAME.height);
            let mut fake = Scripted;
            let probes = [
                Probe::Detect { region: None },
                Probe::Reinforce {
                    region: world::BED.expand(0.1),
                },
                Probe::Reinforce {
                    region: Rect::new(0.0, 0.0, 90.0, 90.0),
                },
                Probe::Reinforce {
                    region: Rect::new(700.0, 500.0, 800.0, 600.0),
                },
                Probe::Faces {
                    region: FaceRegion::Headboard(world::HEADBOARD),
                },
                Probe::Faces {
                    region: FaceRegion::Subject(world::in_bed(0).bbox),
                },
            ];
            for probe in probes {
                let region = probe.region().unwrap_or(Rect::new(0.0, 0.0, 640.0, 480.0));
                origin.set((region.x0.floor(), region.y0.floor()));
                let request = Request {
                    id: LookId(1),
                    probe: probe.clone(),
                };
                let (from_real, _) = real.look(&image, &request);
                let (from_fake, _) = Senses::look(&mut fake, &m, &request);
                assert_eq!(from_real, from_fake, "{faces:?} {probe:?}");
            }
        }
    }
}

#[test]
fn the_binary_runs_s1_and_writes_v3_and_v4_for_rerun() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let output = std::env::temp_dir().join(format!("calipso-s1-{}.jsonl", std::process::id()));
    let status = std::process::Command::new(env!("CARGO_BIN_EXE_calipso-detector"))
        .current_dir(&root)
        .args([
            "--config",
            "config/scene.toml",
            "--scene",
            "golden/s1-staff-put-to-bed.md",
            "--fast",
            "--jsonl",
        ])
        .arg(&output)
        .output()
        .unwrap();
    assert!(
        status.status.success(),
        "scene S1 does not match its expectations:\n{}",
        String::from_utf8_lossy(&status.stdout)
    );
    let validator = |name: &str| {
        let path = root.join(format!("contract/schema/{name}.json"));
        jsonschema::validator_for(
            &serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap(),
        )
        .unwrap()
    };
    let validators: std::collections::HashMap<String, _> = [
        "calipso.scene.v3",
        "calipso.events.v3",
        "calipso.scene.v4",
        "calipso.events.v4",
    ]
    .into_iter()
    .map(|name| (name.to_owned(), validator(name)))
    .collect();
    let text = std::fs::read_to_string(&output).unwrap();
    std::fs::remove_file(&output).ok();
    // The writer writes what is published: with the default profile (R13-5), v3
    // and v4 interleaved, each line against its version's schema.
    let mut count: std::collections::HashMap<String, usize> = Default::default();
    for line in text.lines() {
        let value: serde_json::Value = serde_json::from_str(line).unwrap();
        let schema = value["schema"].as_str().unwrap().to_owned();
        assert!(validators[&schema].is_valid(&value), "{line}");
        *count.entry(schema).or_default() += 1;
    }
    // 25 samples and the transport announcement, in each version; S1's facts.
    assert_eq!(count["calipso.scene.v3"], 26);
    assert_eq!(count["calipso.scene.v4"], 26);
    assert!(count["calipso.events.v3"] > 0 && count["calipso.events.v4"] > 0);
}

#[test]
fn the_binary_rejects_a_profile_without_the_scenes_geometry() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let output = std::env::temp_dir().join(format!("calipso-geo-{}.jsonl", std::process::id()));
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_calipso-detector"))
        .current_dir(&root)
        .args([
            "--config",
            "config/test.toml",
            "--scene",
            "golden/s1-staff-put-to-bed.md",
            "--fast",
            "--jsonl",
        ])
        .arg(&output)
        .output()
        .unwrap();
    std::fs::remove_file(&output).ok();
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("the scenes' geometry"));
}

#[test]
fn a_scene_that_does_not_match_fails_and_names_the_span() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let dir = std::env::temp_dir();
    let scene = dir.join(format!("calipso-bad-{}.md", std::process::id()));
    let output = dir.join(format!("calipso-bad-{}.jsonl", std::process::id()));
    // At the end of 11-15 the resident is held: saying "empty" is false.
    std::fs::write(
        &scene,
        S1.replace(
            "| 2         | occupied, held |",
            "| 2         | empty          |",
        ),
    )
    .unwrap();
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_calipso-detector"))
        .current_dir(&root)
        .args(["--config", "config/scene.toml", "--fast", "--scene"])
        .arg(&scene)
        .arg("--jsonl")
        .arg(&output)
        .output()
        .unwrap();
    std::fs::remove_file(&scene).ok();
    std::fs::remove_file(&output).ok();
    assert!(!out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("✗ span 11-15"), "{stdout}");
    assert!(stdout.contains("✓ span 6-10"), "{stdout}");
}

mod every_scene {
    //! R7-4: each scene in golden/ runs through the real detector with the
    //! profile it asks for (senses), matches its expectations span by span and
    //! is identical to the direct loop. It is the scenes' CI net.
    use super::*;
    use detector::config::Config;
    use golden::expected::Verification;
    use golden::language::Sense;

    fn root() -> std::path::PathBuf {
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    fn profile(scene: &Scene) -> Config {
        let name = if scene.senses.iter().any(|s| *s != Sense::Detect) {
            "config/scene-r6.toml"
        } else {
            "config/scene.toml"
        };
        Config::load(&root().join(name)).unwrap()
    }

    #[test]
    fn each_golden_scene_matches_through_the_detector_and_equals_the_direct_loop() {
        let mut paths: Vec<_> = std::fs::read_dir(root().join("golden"))
            .unwrap()
            .map(|e| e.unwrap().path())
            .filter(|p| p.extension().is_some_and(|e| e == "md") && !p.ends_with("README.md"))
            .collect();
        paths.sort();
        assert!(paths.len() >= 11, "{paths:?}");
        for path in paths {
            let scene = Scene::parse(&std::fs::read_to_string(&path).unwrap())
                .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
            let config = profile(&scene);
            let mut driver = Driver::new(
                config.policy(),
                config.site_map(),
                Scripted,
                Duration::from_secs(10),
            );
            let mut source = SceneSource::new(&scene, Pace::Fast);
            let mut verification = Verification::new(&scene);
            let mut outputs = Vec::new();
            driver.run(&mut source, &CancellationToken::new(), |p| {
                if p.output.scene.republished.is_none() {
                    verification.observe(p.output.at.sequence as u32, &p.output.scene);
                }
                outputs.push(p.output);
            });
            assert!(
                verification.matches(),
                "{}: {:#?}",
                path.display(),
                verification.outcomes()
            );
            let direct = super::direct_with(&scene, config.policy(), config.site_map());
            assert_eq!(outputs, direct, "{}", path.display());
        }
    }
}

#[test]
fn the_binary_rejects_a_scene_that_assumes_senses_the_profile_lacks() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let output = std::env::temp_dir().join(format!("calipso-r6-{}.jsonl", std::process::id()));
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_calipso-detector"))
        .current_dir(&root)
        .args([
            "--config",
            "config/scene.toml",
            "--scene",
            "golden/r6-a-night-with-faces.md",
            "--fast",
            "--jsonl",
        ])
        .arg(&output)
        .output()
        .unwrap();
    std::fs::remove_file(&output).ok();
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("assumes faces"));
}

mod round_trip {
    //! R7-5: a published night is distilled into a scene, and the distilled
    //! scene, run again through the detector, publishes exactly the same. That
    //! way a real night can be carried (without pixels) and replayed without a
    //! camera.
    use super::*;
    use detector::config::Config;
    use detector::publish::PublisherHandle;
    use golden::distill::distill;
    use golden::expected::Verification;
    use golden::language::Sense;
    use scene_core::contract::{Build, Source};

    fn root() -> std::path::PathBuf {
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    fn profile(scene: &Scene) -> Config {
        let name = if scene.senses.iter().any(|s| *s != Sense::Detect) {
            "config/scene-r6.toml"
        } else {
            "config/scene.toml"
        };
        Config::load(&root().join(name)).unwrap()
    }

    /// Runs the scene through the detector: what was published and the night's
    /// v3 JSONL.
    fn run(scene: &Scene, config: &Config) -> (Vec<Output>, String, Verification) {
        let mut driver = Driver::new(
            config.policy(),
            config.site_map(),
            Scripted,
            Duration::from_secs(10),
        );
        let mut source = SceneSource::new(scene, Pace::Fast);
        let (mut publisher, _rx) = PublisherHandle::channel(
            Source {
                site: "golden".into(),
                camera: "scene".into(),
                run_id: "round-trip".into(),
                build: Build {
                    version: "test".into(),
                    git: "test".into(),
                },
            },
            config.profile().unwrap(),
            &config.publish.contracts,
        );
        let mut verification = Verification::new(scene);
        let (mut outputs, mut jsonl) = (Vec::new(), String::new());
        driver.run(&mut source, &CancellationToken::new(), |p| {
            if p.output.scene.republished.is_none() {
                verification.observe(p.output.at.sequence as u32, &p.output.scene);
            }
            outputs.push(p.output.clone());
            for m in publisher.prepare(p.output, p.timing).messages().unwrap() {
                jsonl.push_str(std::str::from_utf8(&m.bytes).unwrap());
                jsonl.push('\n');
            }
        });
        (outputs, jsonl, verification)
    }

    #[test]
    fn each_golden_scene_distilled_and_replayed_publishes_the_same() {
        let mut paths: Vec<_> = std::fs::read_dir(root().join("golden"))
            .unwrap()
            .map(|e| e.unwrap().path())
            .filter(|p| p.extension().is_some_and(|e| e == "md") && !p.ends_with("README.md"))
            .collect();
        paths.sort();
        for path in paths {
            let scene = Scene::parse(&std::fs::read_to_string(&path).unwrap()).unwrap();
            let config = profile(&scene);
            let (original, jsonl, _) = run(&scene, &config);
            let distilled = distill(&jsonl, None, scene.dwell_s)
                .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
            assert_eq!(distilled.run_id, "round-trip");
            let (replayed, _, verification) = run(&distilled.scene, &config);
            assert_eq!(replayed, original, "{}", path.display());
            assert!(
                verification.matches(),
                "{}: {:#?}",
                path.display(),
                verification.outcomes()
            );
            // What was distilled writes and reads back the same.
            let text = golden::language::write(&distilled.scene, "");
            assert_eq!(Scene::parse(&text).unwrap(), distilled.scene);
        }
    }

    #[test]
    fn a_night_with_a_failure_is_not_distilled_and_names_the_sample() {
        let scene = Scene::parse(S1).unwrap();
        let config = profile(&scene);
        let (_, jsonl, _) = run(&scene, &config);
        // Sample 7 arrives with the detector down (504): it was not observed.
        let line = jsonl
            .lines()
            .find(|l| {
                l.contains(r#""schema":"calipso.scene.v3""#) && l.contains(r#""sequence":7,"#)
            })
            .unwrap()
            .to_owned();
        let down = line.replace(
            r#""republished":null"#,
            r#""republished":{"reason":"detect_failed","observed_at":null}"#,
        );
        assert_ne!(down, line);
        let error = distill(&jsonl.replace(&line, &down), None, None).unwrap_err();
        assert!(error.to_string().contains("sample 7"), "{error}");
    }
}
