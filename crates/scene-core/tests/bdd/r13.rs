//! R13 steps (specs 1101 and 1301): contract v4. What `events.v4` and
//! `scene.v4` publish is checked as their consumers (the TUI, the episodes)
//! read it: the contract's JSON.

use cucumber::gherkin::Step;
use cucumber::{given, then};
use serde_json::Value;

use crate::ward::Ward;

/// The night's transitions, in order, as JSON.
fn transitions(ward: &Ward) -> Vec<Value> {
    ward.turns
        .iter()
        .flat_map(|t| &t.output.v4.events)
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

/// A published triple (`[R, B, H]`) as written in the table: `211`.
fn triple(rbh: &Value) -> String {
    rbh.as_array()
        .expect("rbh is an array")
        .iter()
        .map(|c| c.to_string())
        .collect()
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
        .find(|t| t.output.at.sequence == n)
        .unwrap_or_else(|| panic!("there was no turn at sample {n}"))
        .output
}

#[then(
    expr = "the v4 scene of sample {int} says mode {string}, phase {string} and last cell {string}"
)]
fn scene_v4(ward: &mut Ward, n: u64, mode: String, phase: String, last: String) {
    let scene = serde_json::to_value(&*turn(ward, n).v4.scene).expect("the scene serializes");
    let room = &scene["room"];
    assert_eq!(
        (&room["mode"], &room["phase"], &room["last_cell"]),
        (&Value::from(mode), &Value::from(phase), &Value::from(last)),
        "the v4 room at sample {n}: {room}"
    );
}

#[then(expr = "the v3 scene of sample {int} says state {string}")]
fn scene_v3(ward: &mut Ward, n: u64, state: String) {
    let scene = serde_json::to_value(&*turn(ward, n).scene).expect("the scene serializes");
    assert_eq!(scene["room"]["state"], Value::from(state));
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
        .flat_map(|t| &t.output.v4.events)
        .map(|e| serde_json::to_value(e).expect("the event serializes"))
        .collect();
    let changes = room_changes(events);
    assert!(
        changes.contains(&(from.clone(), to.clone())),
        "events.v4 did not publish room_changed from \"{from}\" to \"{to}\": {changes:?}"
    );
}

#[then(expr = "in events.v3 the room changed only once, from {string} to {string}")]
fn v3_once(ward: &mut Ward, from: String, to: String) {
    let events = ward
        .turns
        .iter()
        .flat_map(|t| &t.output.events)
        .map(|e| serde_json::to_value(e).expect("the event serializes"))
        .collect();
    assert_eq!(room_changes(events), vec![(from, to)]);
}

#[then("on every turn, events.v4 brought down to v3 is events.v3 and room.mode gives room.state")]
fn same_night(ward: &mut Ward) {
    assert!(!ward.turns.is_empty(), "there were turns");
    for t in &ward.turns {
        crate::equivalence::check(&t.output);
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
    use scene_core::contract::v4::{Cell, Fv, Rbh};
    let rbh = |text: &str| match counts(text)[..] {
        [r, b, h] => Rbh(r, b, h),
        _ => panic!("\"{text}\": a triple"),
    };
    let fv = match counts(&seen)[..] {
        [f, v] => Fv(f, v),
        _ => panic!("\"{seen}\": a pair"),
    };
    ward.cell = Some(
        Cell::new(rbh(&before), fv, rbh(&after))
            .as_str()
            .to_string(),
    );
}

#[then(expr = "its cell is {string}")]
fn its_cell(ward: &mut Ward, expected: String) {
    assert_eq!(ward.cell.as_deref(), Some(expected.as_str()));
}
