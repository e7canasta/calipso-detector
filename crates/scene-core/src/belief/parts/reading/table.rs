//! The golden table of the parts' reading: every combination of its six
//! facts, one row each, with the reading and whether it is one of the urgent
//! ones (half the dwell). If a rule changed on purpose, the test prints the
//! new table to copy into `contract/parts-reading-table.txt`.

use super::{Facts, Headboard, PartsReading, dwell, read};
use crate::golden;
use crate::units::Seconds;
use serde::Serialize;

const GOLDEN: &str = include_str!("../../../../../../contract/parts-reading-table.txt");

fn wire<T: Serialize>(value: T) -> String {
    golden::name(value).unwrap_or_default()
}

/// The name the golden table writes the headboard with.
fn headboard_name(headboard: Headboard) -> &'static str {
    match headboard {
        Headboard::Unseen => "unseen",
        Headboard::Free => "free",
        Headboard::Partly => "partly",
        Headboard::Lying => "lying",
    }
}

fn table() -> String {
    let mut out = String::from(
        "# The parts' reading. Facts: g groin in bed · b box in bed · headboard (unseen, free, partly,\n\
         # lying: shoulders and face or head at it) · o something out of bed (arms aside) · B whole boxes in bed.\n\
         # g b headboard o B | reading [urgent: half the dwell]\n",
    );
    let bit = |b: bool| if b { '1' } else { '0' };
    let headboards = [
        Headboard::Unseen,
        Headboard::Free,
        Headboard::Partly,
        Headboard::Lying,
    ];
    for n in 0..16u8 {
        let on = |i: u8| n & (1 << (3 - i)) != 0;
        for headboard in headboards {
            let f = Facts {
                groin_in: on(0),
                box_in: on(1),
                headboard,
                out: on(2),
                boxes_in: on(3),
            };
            let reading = read(f);
            let urgent = if dwell(reading, Seconds::TWO).get() < 2.0 {
                " [urgent]"
            } else {
                ""
            };
            out += &format!(
                "{} {} {:<7} {} {} | {}{urgent}\n",
                bit(f.groin_in),
                bit(f.box_in),
                headboard_name(headboard),
                bit(f.out),
                bit(f.boxes_in),
                wire(scene_contract::PartsReading::from(reading))
            );
        }
    }
    out
}

#[test]
fn the_parts_reading_is_the_golden_table() {
    golden::assert_table("parts-reading-table.txt", GOLDEN, &table());
}

#[test]
fn every_reading_but_undecided_is_reachable() {
    let table = table();
    for reading in [
        PartsReading::OutOfBed,
        PartsReading::OnTheEdge,
        PartsReading::Lying,
        PartsReading::TryingToLeave,
        PartsReading::SittingUp,
        PartsReading::SittingInBed,
    ] {
        assert!(
            table.contains(&format!(
                "| {}",
                wire(scene_contract::PartsReading::from(reading))
            )),
            "{reading:?}"
        );
    }
}
