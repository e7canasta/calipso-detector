//! The golden table of layer 1: every combination of what we believed and
//! what we saw, one row each. If a rule changed on purpose, the test prints
//! the new table to copy into `contract/bed-posture-table.txt`.

use super::super::{BedPostureSeen, BedPostureState};
use super::decide;
use serde::Serialize;

const GOLDEN: &str = include_str!("../../../../../../contract/bed-posture-table.txt");

const STATES: [BedPostureState; 3] = [
    BedPostureState::InBed,
    BedPostureState::Lying,
    BedPostureState::SittingUp,
];
const SEEN: [BedPostureSeen; 3] = [
    BedPostureSeen::Held,
    BedPostureSeen::HeadAway,
    BedPostureSeen::Unknown,
];

/// A value as the contract names it.
fn wire<T: Serialize>(value: &T) -> String {
    serde_json::to_value(value)
        .ok()
        .and_then(|v| v.as_str().map(str::to_owned))
        .unwrap_or_default()
}

fn table() -> String {
    let mut out = String::from("# before | seen | after\n");
    for before in STATES {
        for seen in SEEN {
            let after = decide(before, seen);
            out += &format!("{} | {} | {}\n", wire(&before), wire(&seen), wire(&after));
        }
    }
    out
}

#[test]
fn layer_1_decides_as_the_golden_table() {
    let table = table();
    assert!(
        table == GOLDEN,
        "the bed-posture table changed; if on purpose, copy it to contract/bed-posture-table.txt:\n{table}"
    );
}
