//! R4 steps (specs 201–203): the camera sees a sequence by zones and the engine
//! believes what each span says. The tables are golden sequences written by
//! hand.

use cucumber::gherkin::Step;
use cucumber::{given, then, when};
use scene_core::contract::{Event, EventBody, EventKind, RoomState, Zone};

use crate::ward::{Ward, later};

// --- Background

#[given("a room with the bed delimited")]
fn room(ward: &mut Ward) {
    ward.bed();
}

#[given(regex = r"^stabilization requires (\d+(?:\.\d+)?) seconds? of source time$")]
fn stabilization(ward: &mut Ward, secs: f64) {
    ward.policy.persistence.dwell_secs = secs;
}

#[given(regex = r"^samples arrive every (\d+(?:\.\d+)?) seconds? of source time$")]
fn cadence(ward: &mut Ward, secs: f64) {
    ward.cadence_secs = Some(secs);
}

#[given(expr = "the resident came in alone and lay down in samples {int} to {int}")]
fn came_in_and_lay_down(ward: &mut Ward, from: u32, to: u32) {
    for (n, script) in crate::script::came_in_alone(from, to) {
        ward.turn_with(ward.sample(n), script);
    }
}

#[given(expr = "the camera reconnects before sample {int}")]
fn reconnects(ward: &mut Ward, n: u32) {
    ward.reconnections.push(n);
}

#[given("the staff put the resident to bed and left in samples 1 to 25")]
fn staff_put_them_to_bed(ward: &mut Ward) {
    for (n, script) in crate::script::episode(1) {
        ward.turn_with(ward.sample(n), script);
    }
}

// --- The sequence

use golden::language::{Samples, spans_from_cells};

/// The sequence table: one span per row, in the golden scenes' language
/// (`golden::language`). Each sample is driven with what the scene says the
/// senses see, and the expectations are checked at the end of each span.
#[when("the camera sees this sequence:")]
fn sequence(ward: &mut Ward, step: &Step) {
    let table = step.table.as_ref().expect("the sequence carries a table");
    // The script columns (what the scene does not model) are split off before
    // reading the rest with the scene language.
    let (rows, moves) = split_column(&table.rows, SUBJECT_MOVES);
    let (rows, changes_shape) = split_column(&rows, SUBJECT_CHANGES_SHAPE);
    let (rows, pose) = split_column(&rows, POSE);
    let (rows, depth) = split_column(&rows, DEPTH);
    let (rows, bed_depth) = split_column(&rows, BED_DEPTH);
    let spans = spans_from_cells(&rows).unwrap_or_else(|e| panic!("{e}"));
    for (i, span) in spans.into_iter().enumerate() {
        let shift = match moves.get(i).map(String::as_str) {
            None | Some("") => false,
            Some("yes") => true,
            Some(other) => panic!("\"{SUBJECT_MOVES}\": \"yes\" or empty, not \"{other}\""),
        };
        let (pose_fails, figure) = match pose.get(i).map(String::as_str) {
            None | Some("" | "sees") => (false, None),
            Some("fails") => (true, None),
            Some(other) => (false, Some(figure(other))),
        };
        let depth_fails = match depth.get(i).map(String::as_str) {
            None | Some("" | "sees") => false,
            Some("fails") => true,
            Some(other) => panic!("\"{DEPTH}\": \"sees\", \"fails\" or empty, not \"{other}\""),
        };
        let bed_invalid = match bed_depth.get(i).map(String::as_str) {
            None | Some("" | "sees") => false,
            Some("invalid") => true,
            Some(other) => {
                panic!("\"{BED_DEPTH}\": \"sees\", \"invalid\" or empty, not \"{other}\"")
            }
        };
        let reshape = match changes_shape.get(i).map(String::as_str) {
            None | Some("") => false,
            Some("yes") => true,
            Some(other) => {
                panic!("\"{SUBJECT_CHANGES_SHAPE}\": \"yes\" or empty, not \"{other}\"")
            }
        };
        for (n, mut script) in crate::script::turns(&[span]) {
            if reshape {
                ward.reshaped = !ward.reshaped;
            }
            if bed_invalid {
                script.bed = None;
            }
            if depth_fails {
                script.depth = Err(scene_core::look::Failure::Model);
            }
            if shift {
                ward.shifted = !ward.shifted;
            }
            if pose_fails {
                script.pose = Err(scene_core::look::Failure::Model);
            }
            if figure.is_some() {
                script.figure = figure;
            }
            let at = ward.sample(n);
            ward.turn_with(at, script);
        }
        let scene = &ward.last().output.scene;
        let Samples { from, to } = span.samples;
        if let Some(difference) = golden::expected::difference(&span.expected, scene) {
            panic!("at the end of span {from}-{to}, {difference}: {scene:#?}");
        }
    }
}

/// Script column: in each sample of the span, the resident changes place in
/// bed and stays there (801). It lives in the harness and not in
/// `golden::language` while no golden scene needs it.
const SUBJECT_MOVES: &str = "subject moves";

/// Script column: in each sample of the span, the resident changes shape in
/// bed without shifting, and stays so (903).
const SUBJECT_CHANGES_SHAPE: &str = "subject changes shape";

/// Script column: what the pose model answers in the span (801).
const POSE: &str = "pose";

/// A body figure by its name in the table (902).
fn figure(name: &str) -> crate::script::Figure {
    use crate::script::Figure;
    match name {
        "lying" => Figure::Lying,
        "lying, feet covered" => Figure::FeetCovered,
        "an arm out" => Figure::ArmOut,
        "sitting on the edge" => Figure::SittingOnTheEdge,
        "sitting up" => Figure::SittingUp,
        "no torso" => Figure::NoTorso,
        "standing" => Figure::Standing,
        other => panic!("\"{POSE}\": unknown figure \"{other}\""),
    }
}

/// Script column: what the subject's depth model answers (802).
const DEPTH: &str = "depth";

/// Script column: what depth answers at the bed's points (803).
const BED_DEPTH: &str = "bed depth";

/// The table without the `name` column, and that column's values per data row
/// (empty if the table does not have it).
fn split_column(rows: &[Vec<String>], name: &str) -> (Vec<Vec<String>>, Vec<String>) {
    let Some(i) = rows
        .first()
        .and_then(|h| h.iter().position(|c| c.trim() == name))
    else {
        return (rows.to_vec(), Vec::new());
    };
    let values = rows[1..].iter().map(|r| r[i].trim().to_owned()).collect();
    let rest = rows
        .iter()
        .map(|r| {
            r.iter()
                .enumerate()
                .filter(|&(j, _)| j != i)
                .map(|(_, c)| c.clone())
                .collect()
        })
        .collect();
    (rest, values)
}

// --- What is believed

#[then(expr = "the room is {string}")]
fn the_room(ward: &mut Ward, state: String) {
    let expected = match state.as_str() {
        "Empty" => RoomState::Empty,
        "One" => RoomState::One,
        "Several" => RoomState::Several,
        other => panic!("unknown room state: {other}"),
    };
    assert_eq!(ward.last().output.scene.room.state, expected);
}

#[then("there is no subject")]
fn no_subject(ward: &mut Ward) {
    assert_eq!(ward.last().output.scene.subject, None);
}

#[then(regex = r"^there is a subject, (in bed|off the bed)$")]
fn there_is_a_subject(ward: &mut Ward, where_: String) {
    let expected = if where_ == "in bed" {
        Zone::Bed
    } else {
        Zone::Floor
    };
    let subject = ward
        .last()
        .output
        .scene
        .subject
        .as_ref()
        .expect("there is a subject");
    assert_eq!(subject.zone, expected);
}

#[then(expr = "the bed occupant is held since sample {int}")]
fn held_since(ward: &mut Ward, n: u32) {
    let held = &ward.last().output.scene.bed.held;
    assert_eq!(held.len(), 1, "one held: {held:?}");
    assert_eq!(held[0].last_seen, ward.sample(n));
}

// --- What was observed (events)

fn observed(ward: &Ward, which: impl Fn(&EventBody) -> bool) -> Vec<Event> {
    ward.events().filter(|e| which(&e.body)).cloned().collect()
}

pub fn event(which: &str) -> fn(&EventBody) -> bool {
    match which {
        "an entry into the room" => |b| matches!(b, EventBody::Entered),
        "an exit from the room" => |b| matches!(b, EventBody::LeftRoom),
        "an exit from the bed" => |b| matches!(b, EventBody::LeftBed),
        "someone lying down" => |b| matches!(b, EventBody::LayDown),
        "the bed occupant reappearing" => |b| matches!(b, EventBody::Reappeared { .. }),
        other => panic!("unknown event: {other}"),
    }
}

#[then(
    regex = r"^(an entry into the room|an exit from the room|an exit from the bed|someone lying down|the bed occupant reappearing) was observed$"
)]
fn was_observed(ward: &mut Ward, which: String) {
    assert!(
        !observed(ward, event(&which)).is_empty(),
        "{which} was not observed: {:?}",
        ward.events().collect::<Vec<_>>()
    );
}

#[then("no exit from the bed was observed")]
fn no_exit_from_the_bed(ward: &mut Ward) {
    let exits = observed(ward, event("an exit from the bed"));
    assert!(exits.is_empty(), "observed: {exits:?}");
}

#[then(expr = "nobody was observed lying down after sample {int}")]
fn nobody_lay_down_after(ward: &mut Ward, n: u32) {
    let limit = ward.sample(n);
    let late: Vec<_> = observed(ward, event("someone lying down"))
        .into_iter()
        .filter(|e| later(e.at, limit))
        .collect();
    assert!(late.is_empty(), "someone lay down: {late:?}");
}

#[then(expr = "no occupancy event was emitted after sample {int}")]
fn no_events_after(ward: &mut Ward, n: u32) {
    let limit = ward.sample(n);
    let late: Vec<_> = ward
        .events()
        .filter(|e| e.kind == EventKind::Domain && later(e.at, limit))
        .collect();
    assert!(late.is_empty(), "events after sample {n}: {late:?}");
}

// --- Contract v3: expectations on the published language.
#[then(expr = "the bed was observed going from {string} to {string}")]
fn bed_changed(ward: &mut Ward, from: String, to: String) {
    assert!(ward.events().any(|e| {
        let b = serde_json::to_value(&e.body).unwrap();
        b["type"] == "bed_changed" && b["from"] == from && b["to"] == to
    }));
}

#[then(expr = "the room did not change state after sample {int}")]
fn room_did_not_change(ward: &mut Ward, n: u32) {
    assert!(!ward.events().any(|e| later(e.at, ward.sample(n))
        && serde_json::to_value(&e.body).unwrap()["type"] == "room_changed"));
}

#[then(expr = "the {string} event left the bed {string} with {int} seen and {int} held")]
fn state_after(ward: &mut Ward, kind: String, bed: String, seen: u32, held: u32) {
    let e = ward
        .events()
        .find(|e| serde_json::to_value(&e.body).unwrap()["type"] == kind)
        .unwrap();
    let e = serde_json::to_value(e).unwrap();
    assert_eq!(e["after"]["bed"], bed);
    assert_eq!(e["after"]["bed_seen"], seen);
    assert_eq!(e["after"]["bed_held"], held);
}

#[then("the evidence of the last sample has no bodies")]
fn empty_evidence(ward: &mut Ward) {
    let scene = serde_json::to_value(ward.last().output.scene.as_ref()).unwrap();
    assert_eq!(scene["evidence"]["bodies"]["items"], serde_json::json!([]));
}

#[then(regex = r"^there is a pending change at (\d+(?:\.\d+)?) of (\d+(?:\.\d+)?) seconds$")]
fn pending(ward: &mut Ward, have: f64, need: f64) {
    let p = ward.last().output.scene.room.pending.as_ref().unwrap();
    assert_eq!((p.duration.have, p.duration.need), (have, need));
}

#[then(expr = "the bed is {string} since sample {int}")]
fn bed_since(ward: &mut Ward, state: String, n: u32) {
    let bed = serde_json::to_value(&ward.last().output.scene.bed).unwrap();
    assert_eq!(bed["state"], state);
    assert_eq!(bed["since"], serde_json::to_value(ward.sample(n)).unwrap());
}

#[then(expr = "the held occupant was last seen in sample {int}")]
fn last_seen(ward: &mut Ward, n: u32) {
    let bed = serde_json::to_value(&ward.last().output.scene.bed).unwrap();
    assert_eq!(
        bed["held"][0]["last_seen"],
        serde_json::to_value(ward.sample(n)).unwrap()
    );
}
