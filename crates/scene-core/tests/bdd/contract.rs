//! Contract steps (`features/contract`): what `events.v4` and `scene.v4`
//! publish is checked as their consumers read it: the contract's JSON.

use cucumber::gherkin::Step;
use cucumber::{given, then};
use serde_json::Value;

use crate::ward::Ward;

/// The night's transitions, in order, as JSON.
fn transitions(ward: &Ward) -> Vec<Value> {
    ward.turns
        .iter()
        .flat_map(|t| &t.output.v5.events)
        .map(|e| serde_json::to_value(e).expect("the event serializes"))
        .filter(|e| e["kind"] == "transition")
        .collect()
}

/// The night's transition with that cell; if there are several, the last.
fn transition(ward: &Ward, cell: &str) -> Value {
    let all = transitions(ward);
    all.iter()
        .rev()
        .find(|t| t["cell"] == cell)
        .cloned()
        .unwrap_or_else(|| {
            let cells: Vec<&Value> = all.iter().map(|t| &t["cell"]).collect();
            panic!("transition \"{cell}\" was not published; published: {cells:?}")
        })
}

/// The kinds of a transition's facts, comma separated.
fn facts(t: &Value) -> String {
    t["facts"]
        .as_array()
        .expect("a transition carries facts")
        .iter()
        .map(|f| f["type"].as_str().unwrap_or("?"))
        .collect::<Vec<_>>()
        .join(", ")
}

/// A published triple (`room`, `bed`, `held`) as written in the table: `211`.
fn triple(rbh: &Value) -> String {
    let (room, bed, held) = (&rbh["room"], &rbh["bed"], &rbh["held"]);
    format!("{room}{bed}{held}")
}

fn columns(step: &Step) -> (Vec<String>, Vec<Vec<String>>) {
    let table = step.table.as_ref().expect("the step carries a table");
    let mut rows = table.rows.clone();
    let header = rows.remove(0);
    (header, rows)
}

fn cell<'a>(header: &[String], row: &'a [String], name: &str) -> &'a str {
    let i = header
        .iter()
        .position(|c| c == name)
        .unwrap_or_else(|| panic!("the table has no \"{name}\" column"));
    row[i].trim()
}

// --- 1101: the transition is published

#[then("layer 0 published these transitions, in order:")]
fn published_in_order(ward: &mut Ward, step: &Step) {
    let (header, rows) = columns(step);
    let published = transitions(ward);
    let expected: Vec<(String, String, String, String)> = rows
        .iter()
        .map(|r| {
            (
                cell(&header, r, "cell").to_string(),
                cell(&header, r, "before").to_string(),
                cell(&header, r, "after").to_string(),
                cell(&header, r, "facts").to_string(),
            )
        })
        .collect();
    let got: Vec<(String, String, String, String)> = published
        .iter()
        .map(|t| {
            (
                t["cell"].as_str().unwrap_or("?").to_string(),
                t["before"]["mode"].as_str().unwrap_or("?").to_string(),
                t["after"]["mode"].as_str().unwrap_or("?").to_string(),
                facts(t),
            )
        })
        .collect();
    assert_eq!(got, expected, "the night's transitions");
}

#[then(expr = "transition {string} says what we believed lasted {int} seconds of source time")]
fn lasted(ward: &mut Ward, cell: String, secs: u32) {
    let t = transition(ward, &cell);
    assert_eq!(
        t["before"]["lasted_secs"].as_f64(),
        Some(f64::from(secs)),
        "what we believed before \"{cell}\": {}",
        t["before"]
    );
}

#[then(expr = "transition {string} does not say how long what we believed lasted")]
fn does_not_say_how_long(ward: &mut Ward, cell: String) {
    let t = transition(ward, &cell);
    assert!(
        t["before"]["lasted_secs"].is_null(),
        "across sessions the dwell is unknown: {}",
        t["before"]
    );
}

#[then(expr = "layer 0 published transition {string}")]
fn published(ward: &mut Ward, cell: String) {
    transition(ward, &cell);
}

#[then(
    regex = r"^its span started at sample (\d+) and saw (\d+) (?:person|people) far from the bed$"
)]
fn its_span(ward: &mut Ward, from: u64, far: u64) {
    let t = transitions(ward).pop().expect("there was a transition");
    assert_eq!(
        t["seen"]["span"]["first"]["sequence"].as_u64(),
        Some(from),
        "the start of the span: {}",
        t["seen"]
    );
    assert_eq!(
        t["seen"]["context"]["far"].as_u64(),
        Some(far),
        "those who appeared far from the bed: {}",
        t["seen"]
    );
}

#[then(expr = "layer 0 published no transitions since sample {int}")]
fn published_none_since(ward: &mut Ward, from: u64) {
    let after: Vec<Value> = transitions(ward)
        .into_iter()
        .filter(|t| t["at"]["sequence"].as_u64().is_some_and(|s| s >= from))
        .collect();
    assert!(
        after.is_empty(),
        "a flicker publishes nothing; published: {after:?}"
    );
}

// --- 1301: contract v4

#[then(expr = "layer 0 published transition {string} with these facts:")]
fn with_facts(ward: &mut Ward, cell_: String, step: &Step) {
    let t = transition(ward, &cell_);
    let (header, rows) = columns(step);
    let expected: Vec<(String, String, String)> = rows
        .iter()
        .map(|r| {
            (
                cell(&header, r, "fact").to_string(),
                cell(&header, r, "leaves").to_string(),
                cell(&header, r, "mode").to_string(),
            )
        })
        .collect();
    let got: Vec<(String, String, String)> = t["facts"]
        .as_array()
        .expect("a transition carries facts")
        .iter()
        .map(|f| {
            (
                f["type"].as_str().unwrap_or("?").to_string(),
                triple(&f["after"]["rbh"]),
                f["after"]["mode"].as_str().unwrap_or("?").to_string(),
            )
        })
        .collect();
    assert_eq!(got, expected, "the facts of \"{cell_}\"");
}

fn turn(ward: &Ward, n: u64) -> &scene_core::Output {
    &ward
        .turns
        .iter()
        .find(|t| t.output.at.unwrap().sequence == n)
        .unwrap_or_else(|| panic!("there was no turn at sample {n}"))
        .output
}

#[then(
    expr = "the scene of sample {int} says mode {string}, phase {string} and last cell {string}"
)]
fn scene_v4(ward: &mut Ward, n: u64, mode: String, phase: String, last: String) {
    let scene = serde_json::to_value(&*turn(ward, n).v5.scene).expect("the scene serializes");
    let room = &scene["room"];
    assert_eq!(
        (&room["mode"], &room["phase"], &room["last_cell"]),
        (&Value::from(mode), &Value::from(phase), &Value::from(last)),
        "the room at sample {n}: {room}"
    );
}

// --- v5: the same turn, read from the v5 projection (`Output::v5`)

#[then(
    expr = "the v5 scene of sample {int} says mode {string}, phase {string} and last cell {string}"
)]
fn scene_v5(ward: &mut Ward, n: u64, mode: String, phase: String, last: String) {
    let scene = serde_json::to_value(&*turn(ward, n).v5.scene).expect("the v5 scene serializes");
    let room = &scene["room"];
    assert_eq!(
        (&room["mode"], &room["phase"], &room["last_cell"]),
        (&Value::from(mode), &Value::from(phase), &Value::from(last)),
        "the v5 room at sample {n}: {room}"
    );
}

#[then(expr = "the v5 scene of sample {int} has no subject verdict")]
fn v5_has_no_verdict(ward: &mut Ward, n: u64) {
    let scene = serde_json::to_value(&*turn(ward, n).v5.scene).expect("the v5 scene serializes");
    assert!(
        !scene["subject"].is_null(),
        "the v5 scene at sample {n} has no subject to say a verdict of: {}",
        scene["subject"]
    );
    assert!(
        scene["subject"].get("verdict").is_none(),
        "the v5 scene at sample {n} still names a verdict: {}",
        scene["subject"]
    );
}

#[then(expr = "the v5 subject depth of sample {int} is named {string} with {float}")]
fn v5_depth_named(ward: &mut Ward, n: u64, name: String, value: f64) {
    let v5 = serde_json::to_value(&*turn(ward, n).v5.scene).expect("the v5 scene serializes");
    let depth = &v5["subject"]["depth"];
    assert!(
        depth[&name]
            .as_f64()
            .is_some_and(|d| (d - value).abs() < 1e-4),
        "the v5 depth at sample {n} is named {name} with {value}: {depth}"
    );
}

#[then(expr = "the v5 bed calibration of sample {int} is named {string} with {float}")]
fn v5_calibration_named(ward: &mut Ward, n: u64, name: String, value: f64) {
    let v5 = serde_json::to_value(&*turn(ward, n).v5.scene).expect("the v5 scene serializes");
    let calibration = &v5["map"]["calibration"];
    assert!(
        calibration[&name]
            .as_f64()
            .is_some_and(|c| (c - value).abs() < 1e-4),
        "the v5 calibration at sample {n} is named {name} with {value}: {calibration}"
    );
}

#[then(
    expr = "the v5 transition {string} names rbh as room, bed and held with {int}, {int}, {int}"
)]
fn v5_rbh_named(ward: &mut Ward, cell_: String, room: u64, bed: u64, held: u64) {
    let t = transition5(ward, &cell_);
    let rbh = &t["after"]["rbh"];
    assert_eq!(
        (rbh["room"].clone(), rbh["bed"].clone(), rbh["held"].clone()),
        (Value::from(room), Value::from(bed), Value::from(held)),
        "the v5 rbh of \"{cell_}\": {rbh}"
    );
}

#[then(expr = "the v5 transition {string} names fv as floor and bed with {int}, {int}")]
fn v5_fv_named(ward: &mut Ward, cell_: String, floor: u64, bed: u64) {
    let t = transition5(ward, &cell_);
    let fv = &t["seen"]["fv"];
    assert_eq!(
        (fv["floor"].clone(), fv["bed"].clone()),
        (Value::from(floor), Value::from(bed)),
        "the v5 fv of \"{cell_}\": {fv}"
    );
}

/// The v5 transition with `cell`, as its JSON.
fn transition5(ward: &Ward, cell: &str) -> Value {
    let events: Vec<Value> = ward
        .turns
        .iter()
        .flat_map(|t| t.output.v5.events.iter())
        .map(|e| serde_json::to_value(e).expect("the v5 event serializes"))
        .collect();
    events
        .into_iter()
        .find(|e| e["kind"] == "transition" && e["cell"] == cell)
        .unwrap_or_else(|| panic!("no v5 transition \"{cell}\""))
}

/// A version's `room_changed`, as `(from, to)`.
fn room_changes(events: Vec<Value>) -> Vec<(String, String)> {
    events
        .into_iter()
        .filter(|e| e["type"] == "room_changed")
        .map(|e| {
            (
                e["from"].as_str().unwrap_or("?").to_string(),
                e["to"].as_str().unwrap_or("?").to_string(),
            )
        })
        .collect()
}

#[then(expr = "events.v4 published {string} from {string} to {string}")]
fn v4_published(ward: &mut Ward, kind: String, from: String, to: String) {
    assert_eq!(kind, "room_changed", "this step reads room_changed");
    let events = ward
        .turns
        .iter()
        .flat_map(|t| &t.output.v5.events)
        .map(|e| serde_json::to_value(e).expect("the event serializes"))
        .collect();
    let changes = room_changes(events);
    assert!(
        changes.contains(&(from.clone(), to.clone())),
        "events.v4 did not publish room_changed from \"{from}\" to \"{to}\": {changes:?}"
    );
}

#[then("on every turn, each transition agrees with its facts and its scene")]
fn transitions_agree(ward: &mut Ward) {
    assert!(!ward.turns.is_empty(), "there were turns");
    for t in &ward.turns {
        crate::published::check(&t.output);
    }
}

/// A triple or a pair written as `12,0,0`.
fn counts(text: &str) -> Vec<u32> {
    text.split(',')
        .map(|c| {
            c.trim()
                .parse()
                .unwrap_or_else(|_| panic!("\"{text}\": counts separated by commas"))
        })
        .collect()
}

#[given(expr = "a transition from {string} seeing {string} to {string}")]
fn a_transition(ward: &mut Ward, before: String, seen: String, after: String) {
    use scene_contract::{Fv, Rbh};
    let rbh = |text: &str| match counts(text)[..] {
        [r, b, h] => Rbh(r, b, h),
        _ => panic!("\"{text}\": a triple"),
    };
    let fv = match counts(&seen)[..] {
        [f, v] => Fv(f, v),
        _ => panic!("\"{seen}\": a pair"),
    };
    ward.cell = Some(
        scene_core::cell_of(rbh(&before), fv, rbh(&after))
            .as_str()
            .to_string(),
    );
}

#[then(expr = "its cell is {string}")]
fn its_cell(ward: &mut Ward, expected: String) {
    assert_eq!(ward.cell.as_deref(), Some(expected.as_str()));
}
