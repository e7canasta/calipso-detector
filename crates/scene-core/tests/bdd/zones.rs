//! Zone steps (`features/occupancy`): the camera sees a sequence by zones and
//! the engine believes what each span says. The tables are golden sequences
//! written by hand.

use cucumber::gherkin::Step;
use cucumber::{given, then, when};
use scene_contract::Zone;

use crate::published::{Body, Fact, Kind};
use crate::ward::{Ward, later};

// --- Background

#[given("a room with the bed delimited")]
fn room(ward: &mut Ward) {
    ward.bed();
}

#[given(regex = r"^stabilization requires (\d+(?:\.\d+)?) seconds? of source time$")]
fn stabilization(ward: &mut Ward, secs: f64) {
    ward.policy.persistence.dwell_secs = Seconds::new(secs).expect("a time in source seconds");
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

use golden::language::{Pose, Samples, Span, spans_from_cells};
use scene_core::units::Seconds;

/// The sequence table: one span per row, in the golden scenes' language
/// (`golden::language`). Each sample is driven with what the scene says the
/// senses see, and the expectations are checked at the end of each span.
#[when("the camera sees this sequence:")]
fn sequence(ward: &mut Ward, step: &Step) {
    let table = step.table.as_ref().expect("the sequence carries a table");
    // The script columns (what the scene does not model) are split off before
    // reading the rest with the scene language, which reads `subject moves`
    // and `pose` itself.
    let (rows, changes_shape) = split_column(&table.rows, SUBJECT_CHANGES_SHAPE);
    let (rows, depth) = split_column(&rows, DEPTH);
    let (rows, bed_depth) = split_column(&rows, BED_DEPTH);
    let spans = spans_from_cells(&rows).unwrap_or_else(|e| panic!("{e}"));
    for (i, span) in spans.into_iter().enumerate() {
        // The ward keeps where the resident lies across steps: it toggles the
        // shift itself, and the world gets the span without it.
        let shift = span.subject_moves;
        let (pose_fails, figure) = match span.pose {
            Pose::Unscripted => (false, None),
            Pose::Fails => (true, None),
            Pose::Figure(figure) => (false, Some(figure)),
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
        let still = Span {
            subject_moves: false,
            ..span
        };
        for (n, mut script) in crate::script::turns(&[still]) {
            if reshape {
                ward.reshaped = !ward.reshaped;
            }
            if bed_invalid {
                script.bed = None;
            }
            if depth_fails {
                script.depth = Err(scene_look::Failure::Model);
            }
            if shift {
                ward.shifted = !ward.shifted;
            }
            if pose_fails {
                script.pose = Err(scene_look::Failure::Model);
            }
            if figure.is_some() {
                script.figure = figure;
            }
            let at = ward.sample(n);
            ward.turn_with(at, script);
        }
        let scene = &ward.last().output.v5.scene;
        let Samples { from, to } = span.samples;
        if let Some(difference) = golden::expected::difference(&span.expected, scene.as_ref()) {
            panic!("at the end of span {from}-{to}, {difference}: {scene:#?}");
        }
    }
}

/// Script column: in each sample of the span, the resident changes shape in
/// bed without shifting, and stays so.
const SUBJECT_CHANGES_SHAPE: &str = "subject changes shape";

/// A body figure by its name in the table.
pub(crate) fn figure(name: &str) -> crate::script::Figure {
    crate::script::Figure::named(name).unwrap_or_else(|| panic!("unknown figure \"{name}\""))
}

/// Script column: what the subject's depth model answers.
const DEPTH: &str = "depth";

/// Script column: what depth answers at the bed's points.
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

/// How many people the room holds, in the words of the scenarios.
#[then(expr = "the room is {string}")]
fn the_room(ward: &mut Ward, state: String) {
    let occupants = ward.last().output.v5.scene.room.occupants;
    let matches = match state.as_str() {
        "Empty" => occupants == 0,
        "One" => occupants == 1,
        "Several" => occupants >= 2,
        other => panic!("unknown room state: {other}"),
    };
    assert!(
        matches,
        "the room is not \"{state}\": {occupants} occupants"
    );
}

#[then("there is no subject")]
fn no_subject(ward: &mut Ward) {
    assert_eq!(ward.last().output.v5.scene.subject, None);
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
        .v5
        .scene
        .subject
        .as_ref()
        .expect("there is a subject");
    assert_eq!(subject.zone, expected);
}

#[then(expr = "the bed occupant is held since sample {int}")]
fn held_since(ward: &mut Ward, n: u32) {
    let held = &ward.last().output.v5.scene.bed.held;
    assert_eq!(held.len(), 1, "one held: {held:?}");
    assert_eq!(held[0].last_seen, ward.sample(n));
}

// --- What was observed (events)

fn observed(ward: &Ward, which: impl Fn(&Body) -> bool) -> Vec<Fact> {
    ward.events().filter(|e| which(&e.body)).copied().collect()
}

pub fn event(which: &str) -> fn(&Body) -> bool {
    match which {
        "an entry into the room" => |b| matches!(b, Body::Entered),
        "an exit from the room" => |b| matches!(b, Body::LeftRoom),
        "an exit from the bed" => |b| matches!(b, Body::LeftBed),
        "someone lying down" => |b| matches!(b, Body::LayDown),
        "the bed occupant reappearing" => |b| matches!(b, Body::Reappeared { .. }),
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
        .filter(|e| later(e.at.unwrap(), limit))
        .collect();
    assert!(late.is_empty(), "someone lay down: {late:?}");
}

#[then(expr = "no occupancy event was emitted after sample {int}")]
fn no_events_after(ward: &mut Ward, n: u32) {
    let limit = ward.sample(n);
    let late: Vec<_> = ward
        .events()
        .filter(|e| e.kind == Kind::Domain && later(e.at.unwrap(), limit))
        .collect();
    assert!(late.is_empty(), "events after sample {n}: {late:?}");
}

// --- The published language: what a consumer reads.

/// A published value as its JSON name (`seen`, `held`…).
fn name(value: impl serde::Serialize) -> String {
    serde_json::to_value(value)
        .unwrap()
        .as_str()
        .unwrap()
        .to_owned()
}

#[then(expr = "the bed was observed going from {string} to {string}")]
fn bed_changed(ward: &mut Ward, from: String, to: String) {
    assert!(ward.events().any(|e| matches!(
        e.body,
        Body::BedChanged { from: f, to: t } if name(f) == from && name(t) == to
    )));
}

#[then(expr = "the number of people in the room did not change after sample {int}")]
fn occupants_did_not_change(ward: &mut Ward, n: u32) {
    let limit = ward.sample(n);
    let at_limit = ward
        .turns
        .iter()
        .find(|t| t.output.at == Some(limit))
        .expect("a turn at that sample")
        .output
        .v5
        .scene
        .room
        .occupants;
    for t in ward
        .turns
        .iter()
        .filter(|t| later(t.output.at.unwrap(), limit))
    {
        assert_eq!(
            t.output.v5.scene.room.occupants, at_limit,
            "{:?}",
            t.output.at
        );
    }
}

#[then(expr = "the {string} event left the bed {string} with {int} seen and {int} held")]
fn state_after(ward: &mut Ward, kind: String, bed: String, seen: u32, held: u32) {
    let e = ward
        .events()
        .find(|e| e.type_name() == kind)
        .unwrap_or_else(|| panic!("no {kind} event"));
    assert_eq!(name(e.after.bed), bed);
    assert_eq!(e.after.bed_seen, seen);
    assert_eq!(e.after.bed_held, held);
}

#[then("the evidence of the last sample has no bodies")]
fn empty_evidence(ward: &mut Ward) {
    let scene = serde_json::to_value(ward.last().output.v5.scene.as_ref()).unwrap();
    assert_eq!(scene["evidence"]["bodies"]["items"], serde_json::json!([]));
}

#[then(regex = r"^there is a pending change at (\d+(?:\.\d+)?) of (\d+(?:\.\d+)?) seconds$")]
fn pending(ward: &mut Ward, have: f64, need: f64) {
    let p = ward.last().output.v5.scene.room.pending.as_ref().unwrap();
    assert_eq!((p.duration.have, p.duration.need), (have, need));
}

#[then(expr = "the bed is {string} since sample {int}")]
fn bed_since(ward: &mut Ward, state: String, n: u32) {
    let bed = serde_json::to_value(&ward.last().output.v5.scene.bed).unwrap();
    assert_eq!(bed["state"], state);
    assert_eq!(bed["since"], serde_json::to_value(ward.sample(n)).unwrap());
}

#[then(expr = "the held occupant was last seen in sample {int}")]
fn last_seen(ward: &mut Ward, n: u32) {
    let bed = serde_json::to_value(&ward.last().output.v5.scene.bed).unwrap();
    assert_eq!(
        bed["held"][0]["last_seen"],
        serde_json::to_value(ward.sample(n)).unwrap()
    );
}
