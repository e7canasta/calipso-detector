//! Publishing steps (`features/shell/profile.feature`): the profile decides which
//! contract versions are published.
use crate::{
    driver::{detector_profile, publish_samples},
    world::{Shell, models::Model},
};
use cucumber::{given, then, when};
use scene_core::{
    geom::Rect,
    look::{Detection, Label},
};

/// The resident standing off the bed of the synthetic frame (bed 20,20 to 100,80).
const ON_FLOOR: Rect = Rect {
    x0: 115.0,
    y0: 20.0,
    x1: 140.0,
    y1: 90.0,
};

fn publish_profile(w: &mut Shell, contracts: &[&str]) {
    let list: Vec<String> = contracts.iter().map(|c| format!("\"{c}\"")).collect();
    detector_profile(
        w,
        &format!("[publish]\ncontracts = [{}]\n", list.join(", ")),
    );
    w.main = Some(Model::default());
}

#[given(expr = "the profile publishes only contract {string}")]
fn only(w: &mut Shell, contract: String) {
    publish_profile(w, &[&contract]);
}

#[given(expr = "the profile without the {string} section")]
fn without_section(w: &mut Shell, _section: String) {
    detector_profile(w, "");
    w.main = Some(Model::default());
}

#[given(expr = "the profile publishes contracts {string} and {string}")]
fn both(w: &mut Shell, first: String, second: String) {
    publish_profile(w, &[&first, &second]);
}

#[when(regex = r"^the resident enters alone in samples (\d+) to (\d+)$")]
fn enters_alone(w: &mut Shell, first: u64, last: u64) {
    w.model("main").sees(vec![Detection {
        bbox: ON_FLOOR,
        confidence: 0.9,
        label: Label::Person,
    }]);
    w.samples = (first, last);
    let config = w.config().clone();
    w.messages = publish_samples(w, &config, first, last)
        .into_iter()
        .map(|(_, json)| json)
        .collect();
}

#[then(expr = "no {string} or {string} message was published")]
fn none_of(w: &mut Shell, scene: String, events: String) {
    assert!(!w.messages.is_empty());
    for json in &w.messages {
        assert!(
            json["schema"] != scene && json["schema"] != events,
            "{json}"
        );
    }
}

/// What the wall clock measures, which changes from one run to another with the
/// same belief.
const CLOCK: [&str; 3] = ["published_at_ms", "timing", "spent_ms"];

fn without_clock(json: &mut serde_json::Value) {
    match json {
        serde_json::Value::Object(map) => {
            map.retain(|key, _| !CLOCK.contains(&key.as_str()));
            map.values_mut().for_each(without_clock);
        }
        serde_json::Value::Array(items) => items.iter_mut().for_each(without_clock),
        _ => {}
    }
}

/// The v3 messages, without the clock.
fn v3(messages: &[serde_json::Value]) -> Vec<serde_json::Value> {
    messages
        .iter()
        .filter(|json| json["schema"].as_str().is_some_and(|s| s.ends_with(".v3")))
        .map(|json| {
            let mut json = json.clone();
            without_clock(&mut json);
            json
        })
        .collect()
}

#[then("scene.v3 and events.v3 are identical to those of a profile that also publishes v4")]
fn v3_unchanged(w: &mut Shell) {
    let published = v3(&w.messages);
    detector_profile(w, "[publish]\ncontracts = [\"v3\", \"v4\"]\n");
    let (first, last) = w.samples;
    let config = w.config().clone();
    let without: Vec<_> = publish_samples(w, &config, first, last)
        .into_iter()
        .map(|(_, json)| json)
        .collect();
    assert!(
        published
            .iter()
            .any(|json| json["schema"] == "calipso.events.v3")
    );
    assert_eq!(published, v3(&without));
}

fn validator(name: &str) -> jsonschema::Validator {
    let path = format!(
        "{}/../../contract/schema/{name}.json",
        env!("CARGO_MANIFEST_DIR")
    );
    let schema = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    jsonschema::validator_for(&schema).unwrap()
}

/// The messages of a schema, in publishing order.
fn of<'a>(w: &'a Shell, schema: &str) -> Vec<&'a serde_json::Value> {
    w.messages
        .iter()
        .filter(|json| json["schema"] == schema)
        .collect()
}

#[then(expr = "{string} and {string} scenes were published for the same samples")]
fn same_samples(w: &mut Shell, first: String, second: String) {
    for json in &w.messages {
        let schema = json["schema"].as_str().unwrap();
        assert!(validator(schema).is_valid(json), "{json}");
    }
    let samples = |schema: &str| -> Vec<serde_json::Value> {
        of(w, schema)
            .into_iter()
            .map(|json| json["observed_at"].clone())
            .collect()
    };
    let (first, second) = (samples(&first), samples(&second));
    assert!(!first.is_empty());
    assert_eq!(first, second);
}

#[then("events.v4 has its own numbering and its own last_event_seq")]
fn own_numbering(w: &mut Shell) {
    for version in ["v3", "v4"] {
        let events = of(w, &format!("calipso.events.{version}"));
        assert!(!events.is_empty(), "{version} without events");
        for (i, json) in events.iter().enumerate() {
            assert_eq!(json["seq"], i as u64 + 1, "{json}");
        }
        let id = |seq: u64| match version {
            "v3" => format!("bdd-capability-{seq}"),
            _ => format!("bdd-capability-{version}-{seq}"),
        };
        assert!(
            events
                .iter()
                .all(|json| json["event_id"] == id(json["seq"].as_u64().unwrap()))
        );
        // Each scene states the last event of its version published before it.
        let mut last = 0;
        let mut scenes = 0;
        for json in &w.messages {
            if json["schema"] == format!("calipso.events.{version}") {
                last = json["seq"].as_u64().unwrap();
            } else if json["schema"] == format!("calipso.scene.{version}") {
                scenes += 1;
                assert_eq!(json["seq"], scenes, "{json}");
                assert_eq!(json["last_event_seq"], last, "{json}");
            }
        }
    }
}
