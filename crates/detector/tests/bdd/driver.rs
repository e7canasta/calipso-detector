use crate::world::Shell;
use cucumber::{given, then, when};
use detector::{Config, Driver, FixedClock, PublisherHandle};
use scene_contract::Source;
use scene_geom::Rect;
use scene_look::{Detection, Label};
use std::time::Duration;

/// The profile of the synthetic testsrc frame (160×96) the driver decodes,
/// without external IO, with the bed at 20,20 to 100,80 and the given sections.
pub fn detector_profile(w: &mut Shell, sections: &str) {
    let profile = include_str!("../support/profile.toml")
        .replace("resolution = [800, 600]", "resolution = [160, 96]")
        .replace(
            "[[100,100], [500,100], [500,400], [100,400]]",
            "[[20,20], [100,20], [100,80], [20,80]]",
        );
    w.config = Some(Config::parse(&format!("{profile}\n{sections}")).map_err(|e| e.to_string()));
}

#[given("a detector with reinforcement and scripted models")]
fn reinforce_driver(w: &mut Shell) {
    detector_profile(
        w,
        "[senses.reinforce]\nmodel = \"yolo26l.onnx\"\nbed_padding = 0.15\n",
    );
}

/// The resident lying in the synthetic frame's bed.
pub const RESIDENT: Rect = Rect {
    x0: 30.0,
    y0: 30.0,
    x1: 50.0,
    y1: 60.0,
};

#[given("the main model sees the resident still in bed")]
fn resident_still(w: &mut Shell) {
    w.model("main").sees(vec![Detection {
        bbox: RESIDENT,
        confidence: 0.9,
        label: Label::Person,
    }]);
}

/// A published scene, by sample number.
pub fn scene(w: &Shell, sample: u64) -> &serde_json::Value {
    w.scenes
        .get(&sample)
        .unwrap_or_else(|| panic!("there is no scene for sample {sample}"))
}

#[given("a detector with faces, headboard and scripted models")]
fn faces_driver(w: &mut Shell) {
    let profile = include_str!("../support/profile.toml")
        .replace("resolution = [800, 600]", "resolution = [160, 96]")
        .replace(
            "[[100,100], [500,100], [500,400], [100,400]]",
            "[[20,20], [100,20], [100,80], [20,80]]",
        )
        .replace("[map]", "[map]\nheadboard = [20,20,100,35]");
    w.config = Some(Config::parse(&format!("{profile}\n[senses.faces]\nmodel = \"face/yolov12l-face.onnx\"\nmin_confidence = 0.30\n")).map_err(|e| e.to_string()));
}

#[given("the main model sees nobody")]
fn nobody(w: &mut Shell) {
    w.model("main").sees(vec![]);
}

#[given("the face model sees a face at the headboard")]
fn headboard_face(w: &mut Shell) {
    w.model("face").sees(vec![Detection {
        bbox: Rect::new(20.0, 3.0, 40.0, 10.0),
        confidence: 0.9,
        label: Label::Other,
    }]);
}

#[given(
    regex = r"^the main model sees the resident in bed in samples (\d+) to (\d+) and nobody after$"
)]
fn primary_script(w: &mut Shell, first: u64, last: u64) {
    let model = w.model("main");
    for n in 1..=last {
        model.enqueue(Ok(if n >= first {
            vec![Detection {
                bbox: Rect::new(30.0, 30.0, 50.0, 60.0),
                confidence: 0.9,
                label: Label::Person,
            }]
        } else {
            vec![]
        }));
    }
    model.sees(vec![]);
}

#[given("the reinforcement model sees the resident in bed")]
fn reinforces_resident(w: &mut Shell) {
    // Grown bed [8,11,112,89]; local box, before to_frame.
    w.model("reinforcement").sees(vec![Detection {
        bbox: Rect::new(22.0, 19.0, 42.0, 49.0),
        confidence: 0.9,
        label: Label::Person,
    }]);
}

#[when(regex = r"^samples (\d+) to (\d+) arrive$")]
fn samples(w: &mut Shell, first: u64, last: u64) {
    let config = w.config().clone();
    for (n, json) in publish_samples(w, &config, first, last) {
        if json["schema"] == "calipso.scene.v4" {
            w.scenes.insert(n, json.clone());
        }
        w.messages.push(json);
    }
}

/// Drives the samples with the scripted models and returns, for each, the
/// messages the shell publishes with that profile, in order.
pub fn publish_samples(
    w: &Shell,
    config: &Config,
    first: u64,
    last: u64,
) -> Vec<(u64, serde_json::Value)> {
    let mut driver = Driver::new(
        config.policy().expect("a valid profile"),
        config.site_map(),
        w.senses(),
        Duration::from_secs(10),
        FixedClock::new(),
    );
    let source = Source {
        site: config.camera.site.clone(),
        camera: config.camera.id.clone(),
        run_id: "bdd-capability".into(),
        build: detector::build_info(),
    };
    let (mut publisher, _receiver) = PublisherHandle::channel(source, config.profile().unwrap());
    let mut published = Vec::new();
    for n in first..=last {
        for model in [&w.main, &w.reinforce, &w.faces].into_iter().flatten() {
            model.at(n);
        }
        if let Some(model) = &w.pose {
            model.at(n);
        }
        if let Some(model) = &w.depth {
            model.at(n);
        }
        for processed in driver.arrive(crate::support::sample(n, 1)) {
            let publication = publisher.prepare(processed.output, processed.timing);
            for message in publication.messages().unwrap() {
                published.push((n, serde_json::from_slice(&message.bytes).unwrap()));
            }
        }
    }
    published
}

#[then(regex = r"^the reinforcement model was called (\d+) time, in sample (\d+)$")]
fn reinforce_once(w: &mut Shell, count: usize, sample: u64) {
    let calls = w.model("reinforcement").calls();
    assert_eq!(calls.len(), count);
    assert_eq!(calls[0].sample, sample);
    assert_eq!(calls[0].size, (104, 78));
}

#[then(expr = "the scene of sample {int} says the bed is {string}")]
fn bed(w: &mut Shell, sample: u64, expected: String) {
    assert_eq!(w.scenes[&sample]["body"]["bed"]["state"], expected);
    assert_eq!(w.scenes[&sample]["body"]["bed"]["seen"], 1);
    assert_eq!(
        w.scenes[&sample]["body"]["evidence"]["bodies"]["items"][0]["source"],
        "reinforcement"
    );
}

/// Each message validates against the schema it names.
#[then("every published message validates against its schema")]
fn valid_publications(w: &mut Shell) {
    assert!(w.messages.iter().any(|m| m["schema"] == "calipso.scene.v4"));
    let validator = |name: &str| {
        let path = format!(
            "{}/../../contract/schema/{name}.json",
            env!("CARGO_MANIFEST_DIR")
        );
        let schema = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
        jsonschema::validator_for(&schema).unwrap()
    };
    let validators: std::collections::HashMap<&str, _> = ["calipso.scene.v4", "calipso.events.v4"]
        .into_iter()
        .map(|name| (name, validator(name)))
        .collect();
    for json in &w.messages {
        let schema = json["schema"].as_str().expect("schema");
        assert!(validators[schema].is_valid(json), "{json}");
    }
}

#[then(expr = "the face model was called {int} times")]
fn face_calls(w: &mut Shell, count: usize) {
    let calls = w.model("face").calls();
    assert_eq!(calls.len(), count);
    assert_eq!(
        calls.iter().map(|c| c.sample).collect::<Vec<_>>(),
        (1..=count as u64).collect::<Vec<_>>()
    );
    assert!(calls.iter().all(|c| c.size == (80, 15)));
}

#[then(expr = "the scene of sample {int} carries {int} accepted face")]
fn accepted_face(w: &mut Shell, sample: u64, count: usize) {
    let faces = w.scenes[&sample]["body"]["evidence"]["faces"]["items"]
        .as_array()
        .unwrap();
    assert_eq!(faces.len(), count);
    assert_eq!(faces[0]["on_subject"], false);
    assert_eq!(
        faces[0]["bbox"],
        serde_json::json!({"x0":40.0,"y0":23.0,"x1":60.0,"y1":30.0})
    );
}
