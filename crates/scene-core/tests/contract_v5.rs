//! The equivalence of v5 and v4 (C5): on every turn of the examples, the v5
//! scene and events, read back to v4, are the v4 ones, except what v5 changes on
//! purpose (ADR "Lo que espera el contrato v5"). The list of changes is explicit
//! below: a difference outside it fails the test.

// Each test crate that includes the script uses its own subset of it.
#[allow(dead_code)]
mod script;
use scene_contract::{FrameInfo, SampleRef, SessionId, SourceSecs};
use scene_core::map::SiteMap;
use scene_core::policy::Policy;
use scene_core::units::Seconds;
use scene_core::{Input, Monitor};
use scene_geom::{Quad, Rect};
use scene_look::Spent;
use serde_json::{Value, json};

/// What v5 changes on purpose, by the JSON path it changes in the scene and the
/// events. Anything else that differs is a failure.
const CHANGED_ON_PURPOSE: &[&str] = &[
    // change 3: the subject has no verdict.
    "subject.verdict",
    // change 4: the depth and the calibration reference, relative.
    "subject.depth.meters",
    "map.calibration.reference_m",
    // change 5: rbh and fv with names.
    "rbh",
    "fv",
];

/// Reads a v5 JSON back to v4 names: the inverse of the five changes. What v5
/// names differently, v4 names the old way.
fn v5_as_v4(mut value: Value) -> Value {
    fn rename(value: &mut Value) {
        match value {
            Value::Object(map) => {
                if let Some(depth) = map.get_mut("depth")
                    && let Some(relative) = depth.get("relative").cloned()
                    && let Some(d) = depth.as_object_mut()
                {
                    d.remove("relative");
                    d.insert("meters".into(), relative);
                }
                if let Some(cal) = map.get_mut("calibration")
                    && let Some(reference) = cal.get("reference").cloned()
                    && let Some(c) = cal.as_object_mut()
                {
                    c.remove("reference");
                    c.insert("reference_m".into(), reference);
                }
                if let Some(rbh) = map.get("rbh").cloned()
                    && rbh.is_object()
                {
                    map.insert("rbh".into(), json!([rbh["room"], rbh["bed"], rbh["held"]]));
                }
                if let Some(fv) = map.get("fv").cloned()
                    && fv.is_object()
                {
                    map.insert("fv".into(), json!([fv["floor"], fv["bed"]]));
                }
                for v in map.values_mut() {
                    rename(v);
                }
            }
            Value::Array(items) => items.iter_mut().for_each(rename),
            _ => {}
        }
    }
    rename(&mut value);
    value
}

/// The paths where two JSON values differ, ignoring the ones v5 changes on
/// purpose.
fn differences(a: &Value, b: &Value, path: &str, out: &mut Vec<String>) {
    if a == b {
        return;
    }
    match (a, b) {
        (Value::Object(x), Value::Object(y)) => {
            let keys: std::collections::BTreeSet<_> = x.keys().chain(y.keys()).collect();
            for k in keys {
                let p = if path.is_empty() {
                    k.clone()
                } else {
                    format!("{path}.{k}")
                };
                differences(
                    x.get(k).unwrap_or(&Value::Null),
                    y.get(k).unwrap_or(&Value::Null),
                    &p,
                    out,
                );
            }
        }
        _ => {
            if !CHANGED_ON_PURPOSE.iter().any(|c| path.ends_with(c)) {
                out.push(format!("{path}: v5 read back {a} != v4 {b}"));
            }
        }
    }
}

fn observe(monitor: &mut Monitor, sequence: u64, script: &script::Script) -> scene_core::Output {
    monitor
        .handle(Input::Sample {
            at: SampleRef {
                session: SessionId(1),
                sequence,
                source_secs: SourceSecs(sequence as f64),
            },
            frame: FrameInfo {
                width: 640,
                height: 480,
            },
        })
        .drive(|request| (script.respond(request), Spent(1.0)))
}

#[test]
fn every_turn_of_the_examples_reads_back_to_v4_except_what_v5_changes() {
    for n in 1..=8 {
        let mut policy = Policy::default();
        policy.persistence.dwell_secs = Seconds::TWO;
        let mut monitor = Monitor::new(
            policy,
            SiteMap::new(Quad::from_rect(Rect::new(100.0, 100.0, 500.0, 400.0))),
        );
        for (sequence, script) in script::episode(n) {
            let out = observe(&mut monitor, u64::from(sequence), &script);
            let v4 = serde_json::to_value(&*out.scene).expect("the v4 scene serializes");
            let v5 =
                v5_as_v4(serde_json::to_value(&*out.v5.scene).expect("the v5 scene serializes"));
            let mut diffs = Vec::new();
            differences(&v5, &v4, "", &mut diffs);
            assert!(
                diffs.is_empty(),
                "episode {n}, sample {sequence}: v5 reads back differently from v4: {diffs:?}"
            );
            assert_eq!(
                out.v5.events.len(),
                out.events.len(),
                "episode {n}, sample {sequence}: v5 publishes a different number of events"
            );
        }
    }
}
