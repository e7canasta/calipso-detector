//! The golden table of layer 1: every combination of what layer 1 believed,
//! what layer 0 believes now and what layer 1 perceived, one row each. Held
//! or without layer 0, layer 1 perceives nothing of its own (`—`). If a rule
//! changed on purpose, the test prints the new table to copy into
//! `contract/bed-posture-table.txt`.

use super::super::{BedPostureState, Sighting};
use super::decide;
use crate::belief::InBed;
use crate::golden;
use serde::Serialize;

const GOLDEN: &str = include_str!("../../../../../../contract/bed-posture-table.txt");

const BEFORE: [Option<BedPostureState>; 5] = [
    None,
    Some(BedPostureState::InBed),
    Some(BedPostureState::Lying),
    Some(BedPostureState::SittingUp),
    Some(BedPostureState::GettingOut),
];
const SIGHTINGS: [(Sighting, &str); 6] = [
    (Sighting::HeadboardLying, "headboard_lying"),
    (Sighting::HeadboardUpright, "headboard_upright"),
    (Sighting::LegsOut, "legs_out"),
    (Sighting::HeadSide, "head_side"),
    (Sighting::HeadAway, "head_away"),
    (Sighting::Unknown, "unknown"),
];

/// A value as the contract names it; nothing is `—`.
fn wire<T: Serialize>(value: Option<T>) -> String {
    value
        .and_then(golden::name)
        .unwrap_or_else(|| "—".to_owned())
}

fn table() -> String {
    let mut out = String::from("# l1 before | l0 now + seen | l1 after\n");
    for before in BEFORE {
        let mut row = |input: String, l0, sighting| {
            let after = decide(before, l0, sighting);
            out += &format!("{} | {input} | {}\n", wire(before), wire(after));
        };
        row("—".to_owned(), None, Sighting::Unknown);
        row("held".to_owned(), Some(InBed::Held), Sighting::Unknown);
        for (sighting, name) in SIGHTINGS {
            row(format!("seen + {name}"), Some(InBed::Seen), sighting);
        }
    }
    out
}

#[test]
fn layer_1_decides_as_the_golden_table() {
    golden::assert_table("bed-posture-table.txt", GOLDEN, &table());
}
